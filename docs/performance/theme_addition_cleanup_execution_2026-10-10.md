# Theme Addition Cleanup Execution — October 10, 2026

This record describes the implementation of the [addition-necessity cleanup
plan](../plans/2026-10-10-1535-refactor-theme-addition-necessity-plan.md). It separates
removed executable machinery, simplified current algorithms, retained contracts and
unchanged static data. Source-line reductions do not establish a binary-size or
latency improvement. Final validation and cost measurements remain open below.

**Completion correction at `a987774ff`:** the first implementation batch passed the local
owners recorded below, but the overall cleanup plan is not complete. The subsequent
[remaining-code review](theme_remaining_code_review_2026-10-10.md) found unreachable prepared
catalog consumers, a production-inaccessible Core overlay, redundant receipt work and a
Sequence exact-declaration proof gap. U2, U4–U7 and U9 are reopened in the plan. These findings
do not invalidate the earlier source-bound test results; those results do not establish
complete retirement or minimality. No new implementation or runtime measurement is included
in that review, and affected remote owner checks remain unverified here.

## Accounting boundaries

The original PR inventory compares main
`2928265122d5a3d35e239e3ddbce885c7e342c59` with implementation baseline
`8664ff06ea234da73c59de99fb12a76034e301d3`. Its [107-module
index](theme_pr_addition_necessity_review_2026-10-10.md#disjoint-module-accounting)
assigns all 1,683 changed paths once, including 104 binary paths: **421,112 added
and 40,358 deleted text lines**. Module counts include their tests; the independent
test view must not be added to those counts.

The implementation snapshot here compares that frozen implementation baseline
with tree `cbddfdf9ae305a57b39b08503d3c9c765edeb1a5`, captured while local HEAD was
`eaebdbe52e0c180438dac13af8d5a9ad7a81f63f`. The tree includes both committed cleanup
and tracked/untracked working changes. A temporary Git index captured the tree
without changing the real index or working files. Git's NUL-delimited numstat with
rename detection supplies the denominator; ignored build output, temporary test
snapshots and this report itself are excluded. Later implementation changes require
a refreshed count rather than reusing this snapshot as a final receipt.

At this snapshot, **135 paths** change: **2,553 added, 18,691 deleted**, net
**16,138 fewer text lines**. Four byte-identical scene relocations contribute zero
added/deleted lines. No changed binary paths enter this cleanup denominator.

| Disjoint file classification | Added | Deleted | Net | Paths |
| --- | ---: | ---: | ---: | ---: |
| Rust shared source, including inline tests | 1,308 | 12,535 | -11,227 | 73 |
| Rust test-only files and benchmark | 296 | 5,939 | -5,643 | 19 |
| Non-Rust independent tests | 15 | 27 | -12 | 3 |
| Host source and tools | 8 | 28 | -20 | 8 |
| Workspace/dependency metadata | 0 | 47 | -47 | 3 |
| Generated legal metadata | 17 | 17 | 0 | 17 |
| Documentation and durable execution records | 873 | 98 | +775 | 7 |
| Static fixtures and provenance | 36 | 0 | +36 | 5 |
| **Total** | **2,553** | **18,691** | **-16,138** | **135** |

The static row includes a new 36-line owner README and the four zero-line moves;
it does not represent 36 lines of new render functionality. The workspace row
includes deletion of the retired crate's 30-line manifest, one member entry and
its 16-line lock entry. The legal row updates 17 Cargo-lock digest fields; it
introduces no dependency or attribution change.

A second, overlapping view separates recognized Rust test bodies from shared
source. Rust files contribute **1,604 additions and 18,474 deletions**. Independent
`tests`/`tests.rs` files, the affected benchmark and recognizable trailing inline
`mod tests` bodies account for **705 additions and 7,202 deletions**, net **6,497
fewer recognized Rust test lines**. The remaining **899 additions and 11,272
deletions**, net **10,373 fewer shared Rust lines**, include production/private
machinery and test helpers outside those recognizable spans. This is a
conservative test-body classification, not a claim that every remaining line is
production code. It avoids presenting deleted inline tests as production savings.

The direct main-to-snapshot diff is **405,029 additions and 40,413 deletions**, net
**364,616**, across 1,669 paths, still including 104 binary paths. It is computed
directly rather than subtracting two independently generated addition counts.
The cleanup reduces the original PR's net text delta by 16,138 lines, **4.24%** of
380,754. It does not justify describing all remaining additions as minimal.

## Major contributors to the reduction

The complete ownership index remains the original 107-module report. This
snapshot changes paths assigned to 55 of those modules. The following table
highlights the main reductions rather than introducing a second overlapping
107-module partition.

| Existing module owner | Added | Deleted | Net | Interpretation |
| --- | ---: | ---: | ---: | --- |
| `crates/merman-theme-fixtures` | 0 | 10,440 | -10,440 | Remove private catalog/parser, its tests and manifest; four static scenes move unchanged |
| `render/diagram_theme` | 294 | 4,011 | -3,717 | Historical classification/projection tests and canonical case construction |
| `render/infrastructure` | 229 | 1,896 | -1,667 | Retire generic cutover chain; retain Architecture's independent text receipt |
| `crates/merman-export` | 81 | 493 | -412 | Remove unreachable private embedded-font success machinery |
| `render/text` | 3 | 293 | -290 | Remove unreachable catalog-backed prepared-text construction |
| `render/treemap`, `render/venn`, `render/zenuml` | 95 | 581 | -486 | Single-title rule mechanics move to a shared current owner |
| `render/family` | 286 | 95 | +191 | Includes new 207-line single-title owner and independent evidence tests |
| `render/flowchart` | 118 | 170 | -52 | Share local scalar/padding/dash/typography and incomplete-facet accounting |
| `render/integration-tests` | 217 | 242 | -25 | Preserve behavioral cases; simplify local setup and independent observations |

The single-title families and `render/family` must be read together: the new owner
is not free, and deleting duplicated callers alone overstates the reduction.
Likewise, the catalog's 10,440-line total includes 3,056 lines of independent
Rust tests, 7,354 lines in source files, and its 30-line manifest. Its source files
also contain inline tests. This was a private self-consuming fixture execution
surface, not 10,440 lines of user-facing rendering removed.

## Implementation dispositions

### U1 — Complete the necessity and ownership decisions

**Retain responsibilities with explicit local simplifications.** The original
107-module table remains the complete index for the frozen PR, including docs,
assets, packaging and adjacent release changes. Retain-responsibility entries
continue to own their documented consumer; retain-history/data entries continue
to preserve useful dated evidence, attribution and provenance. Mixed entries are
resolved through U2–U9 or the specific retain decisions below. This does not turn
representative review into a line-by-line proof of 421,112 additions.

- **GitGraph fallback: retain.** BestEffort static fill still emits when an ordinal
  or unsupported sibling cannot be certified. Strict rejection, shadowed rules,
  source `commitLineColor` and separate `lineColor` ownership remain meaningful.
- **Graph-label preparation: retain.** Flowchart's measured/wrapped labels use
  prepared ranges with source entities, Unicode expansion and repeated visible
  text. A generic final text count does not reproduce source-to-visible mapping.
- **Shape writers: retain.** Concrete SVG/HTML/composite shapes, dispatch and
  actual terminal receipts remain distinct owners. No universal shape framework
  replaces them merely to reduce line counts.
- **Remaining families: retain concrete terminals.** Different role counts, CSS
  surfaces, source precedence, variants and geometry are actual contracts. U2
  removes dead enum consumers across these modules without consolidating their
  writer evidence into plan-only assertions.
- **Public recipe/contract and host surfaces: retain.** Mermaid compatibility
  derivation is an input boundary. Structured recipes, presets, support discovery,
  transport and native projection own separate user contracts.
- **Scripts and adjacent workflows: retain their bounded owners.** The original
  script inventory adds 1,912 lines and deletes 109 across 33 paths. Qualification,
  actual archive/source/executable checks, immutable bundles, public-boundary
  consumers and release/security checks remain relevant. The PR must describe
  adjacent release work separately. The external Rust consumer invokes `rustc`;
  this cleanup does not expand scripts into a Rust or CSS compiler front end.

### U2 — Historical cutover and third classification state

**Retire** generic route cutover descriptors, projection/report plumbing,
self-referential catalog assertions and `theme_route_cutover.rs`. **Retain** the
actual Architecture text observation as `architecture_text_receipt.rs`, keeping
its independent expected/actual facts, artifact integrity and original digest
domain. The public-boundary probe now names that live private receipt, so deleting
the old type does not make the probe vacuous.

**Simplify** runtime route classification to typed support or Unsupported.
`LegacyCompatibility`, its summary bit, dead fallback dictionaries and unreachable
consumer match arms are removed. This does not remove GitGraph's actual static-fill
BestEffort behavior.

Before/after temporary snapshots enumerate selector shapes, values, paint facets,
typography, ordinal/effect routes and public support summaries. Both contain
**326,625 records**, **39,925,749 bytes**, SHA-256
`5ea65f7bb69c0daa5d8e70a01c1c2a2b2015d255cca9ac315f217ee643d41665`, and compare
byte-for-byte equal. The temporary snapshot test was removed after this bounded
transition check; permanent tests retain current behavior rather than legacy
storage layout.

### U3 — Private catalog and static assets

**Retire** `merman-theme-fixtures`, including its alternate CSS/directive parser,
wire/catalog execution, self-consumed acceptance model and lineage-only tests.
Remove its workspace/lock entries. **Retain** the independent authoring fixture
crate and real native preset qualification.

**Move** public Cyberpunk scenes into `fixtures/themes/public-cyberpunk/`; update
current renderer/facade tests, benchmark, qualification, Playground, Node, Apple,
Python and debug-tool consumers. All **106 pre-existing `fixtures/themes` files**
remain byte-identical to the frozen baseline, as do the **four moved files**.
The 24 reference themes, 18 translations, pinned paths/revisions/hashes, font
provenance and MIT/OFL/NOTICE material remain data owned by the static directory.
Deleting a catalog reader does not delete that source evidence.

### U4 — Unreachable font and prepared-text success state

**Simplify** the private successful export font catalog to the asset-free system
case. **Retire** embedded loading/indexing/alias/provenance closure and private
catalog-backed text-backend construction whose legal path always ends in
`EmbeddedFontsUnavailable`. Remove the corresponding unused private environment
and deferred host-construction machinery.

**Retain** readable font wire modes/report fields, explicit rejection, resource
and error order, system discovery/fallback, glyph queries, final-tree provenance,
public host/session interfaces, cancellation and current text projection seams.
No embedded-font or Portable typography capability is newly advertised.

This batch did not finish U4. `PreparedTextLayout` now has no constructor, yet its private
candidate/fallback state and State/Flowchart/Sequence consumers remain. Session layout/error
slots are always `None`, catalog maps are permanently empty, and export still has an
unreachable exact asset-face matcher. The remaining review specifies the full retirement
boundary; ordinary source projection, prepared labels, host measurement and math remain live.

### U5 — Share local algorithms with current consumers

**Simplify** Flowchart's repeated incomplete-facet accounting and scalar/padding/
dash/typography outcome mechanics, preserving first-residual order and distinct
node/edge paint/effect/ordinal rules. **Share** Sequence effect preparation while
keeping terminal roles and actual effect receipts separate. **Share** Kanban's
expected/actual run-accumulation algorithm while keeping their observations and
CSS/source/geometry facts independent.

**Share** one-title static-fill resolution/accounting for Treemap, Venn and ZenUML.
Each family still supplies title presence/source ownership and proves its actual
writer receipt before recording a terminal. Pending keys, terminal sealing,
unsupported siblings, variants and rule order remain explicit. ZenUML's earlier
non-Title residual is preserved.

**Retain** Cynefin/Wardley font counters: their different CSS surfaces and source
ownership would survive the proposed extraction, while the small shared count
would add indirection without removing a meaningful owner. **Retain** the distinct
facade completion/standalone admission contracts. A subsequent local helper shares
only the identical portability, public/native digest and admission assembly in
`render/document.rs`. Document completion retains Arc-owned evidence without SVG
strict rejection; direct SVG retains owned evidence and enforces its requirement
after admission. Resource fingerprints remain stored-value reads, so this does
not establish fewer render operations or a measured latency improvement.

### U6 — Decode within a boundary and retire unpublished share input

**Simplify** binding selector decoding after existing union validation: a validated
Value is decoded once through the normal typed path. **Retain** raw JSON size/depth/
duplicate/schema checks before allocation, typed semantic validation, complete-spec
preflight, stricter-session re-admission, and layout/CSS/error ordering. These are
different admission contracts, not redundant checks on a single trusted object.

**Retire** the open-branch-only `renderViewportMode` share input across legacy
Base64, s2, s3 and query decoding. **Retain** published versions, current recipes,
legacy host locking and default-to-auto behavior. Unknown retired input is rejected
instead of silently reviving unpublished compatibility.

### U7 — Artifact observation, filters, budgets and comparator

**Retain** State's post-writer observer. Existing final XML traversal does not
provide occurrence identity, HTML overrides, special inner shapes, source ownership
and writer ranges; substituting an expected plan would weaken real emission proof.
No equivalent simpler observer was demonstrated.

**Retain** final SVG validation at distinct artifact/trust boundaries, including
external mutation and prepared math. No immutable artifact identity plus reusable
validation-fact owner was established; introducing a memoization/hash framework
just to avoid these checks would add a new contract.

**Retain** raw-SVG versus usvg-tree native filter checks: units, references, regions
and stages can differ after parsing. **Retain** bounded private SVG observation and
family-local xtask normalization with unknown-syntax rejection and positive/negative
cases; no new general CSS interpreter or semantic fixture normalization is added.

**Retain** Treemap's pre-allocation size estimator, checkpoints and saturating
accounting. Nested/flat JSON construction can amplify allocation; counting bytes
only after creating the expanded Value would move the check past its purpose.

### U9 — Test structure and independent behavior

**Simplify** Canonical baseline/case construction, State SVG-only setup and
Sequence source/site execution helpers. Preserve case names and behavioral
observations; do not replace them with production-derived support expectations.
Sequence checks use independent pinned expectations and the actual target/CSS
property rather than only searching the entire SVG for a matching color.

**Retain** GitGraph's existing execution helpers and separate unsupported-font
cases. Fill-only source ownership and stroke/fill combinations configure different
property owners. Each still needs an actual BestEffort render and Strict failure;
putting their expectations into a universal helper would obscure those differences.
No arbitrary removal of repeated case data or full test DSL is introduced.

### U8 — Reconcile evidence and current documentation

**Update** active corpus/coverage/qualification descriptions and fixture consumers;
keep useful dated history with its original baseline. This execution record
supersedes blanket statements that generic cutover machinery was already retired
while it was still present. **Retain** original artifact budgets until fresh owner
measurements justify a separate adjustment. A source count, removal of a workspace
crate and successful tests do not substitute for rebuilt artifact evidence.

## Coverage mapping and verification receipts

| Boundary | Preserved or strengthened observation |
| --- | --- |
| Canonical recipe identity | 102 intentional field-sensitivity cases; independent identity comparisons, ordering, negative zero and schema/resource cases remain |
| Sequence appearance/source precedence | 137 named tests remain; 8 explicit cases run at site/source (16 executions), 91 derived cases run at site/source (182 executions); actual selector/property observations and independent pinned values |
| State appearance and geometry | 72 named tests remain; SVG-only helper avoids discarded JSON round trips, geometry tests still obtain their layout |
| Current route classification | 326,625 before/after records with identical bytes and SHA-256 above; temporary transition test removed |
| Current artifact sealing | Finalized SVG receipt proves the original document and rejects another document or modified SVG; native prepared receipt rejects public SVG, other native document and mutated native bytes |
| Private API boundary | External consumer names live `ArchitectureTextReceipt`, rather than a type already deleted |
| Static references and public scenes | 106 retained files plus four relocations verified byte-for-byte against the frozen baseline |
| Native preset support | Independent current SVG/PNG qualification and host-dependent evidence retained; 158 integrated tests passed |
| Font/resource rejection | System-only fallback and rejection order retained, no successful embedded-font path introduced |
| Binding/share transport | Raw/schema/resource/error-priority cases and s2/s3 behavior remain distinct from renderer/native observations |

The static and route snapshot comparisons above were directly inspected for this
record. The case mapping records retained test structure, not a claim that every
listed suite or host has passed on the final tree. Debug, private acceptance,
optimized evidence, real native export, browser and language consumers exercise
different boundaries; matching names do not make their jobs redundant.

## Final validation and cost receipts

All listed owners now have commands, feature surfaces, results and the verified source
revision. Remote operating-system matrices remain the responsibility of the existing
CI owners and are not inferred from these local receipts.

| Required owner | Final result |
| --- | --- |
| Full affected renderer/facade/export/preset tests with locked dependencies | 4,789 renderer tests passed (6 skipped), `all-diagrams,layout-cytoscape,layout-elk`, private acceptance cfg; 158 facade/export/native-preset tests passed; 91 all-feature facade document/operation tests passed after sealing sharing; 78 binding theme/resource tests passed |
| Formatting and scoped strict Clippy | Final `cargo fmt --all -- --check`, `git diff --check`, Python AST and Node syntax checks passed; final CLI all-target/all-feature Clippy passed |
| External private-API consumer and optimized evidence rejection | Production external consumer compiled with every producer feature and rejected all private/retired imports. Release/private renderer evidence-support tests: 13 passed, 1 ignored |
| Full blocking SVG structure/parity/parity-root comparator owners | Full locked release `compare-all-svgs --check-dom --dom-modes structure,parity,parity-root --dom-decimals 3 --diagnostic-browser-text-layout --report-root` passed after rebuilding xtask/render/facade/export. Reports resolve the current workspace; existing exact browser/parser residual receipts remain explicit |
| Feature matrix and representative artifact dependency closures | `verify-feature-matrix` passed all 45 build combinations and isolated consumers; `verify_artifact_dependency_closures.py --representative-targets` passed |
| Fresh Web/Typst/native artifact measurements against unchanged budgets | Five fresh Web packages passed 20 budget checks; fresh Typst publish package passed all 4 checks. Native CLI descriptor recipe built and measured below. No budget changes or causal size-improvement claim |
| Actual browser and affected language fixture consumers | Current-source Web five-package/runtime/DOM smoke passed. Rebuilt Playground: 47 Chromium theme tests and 46 Firefox/WebKit terminal tests passed. Native CLI rendered migrated scenes to SVG/PNG. Typst 0.15.0 compiled 23 fixtures and rejected 9 expected failures. Installed darwin-arm64 Node N-API smoke passed; Python installed wheel smoke passed; Apple XCFramework and Swift smoke passed |
| First-batch independent correctness and maintainability review | The scoped reviewers returned no findings for that implementation. Later whole-PR call-chain review found the additional debt linked above; the earlier review is not a global remaining-code signoff |
| Accounting after the first batch and its documentation | The excluding-report snapshot is recorded below. At documentation HEAD `a987774ff`, the full cleanup is 138 paths, 2,991 additions and 18,726 deletions, including this report |

Only the private catalog workspace/lock entry is removed at this stage; no new
third-party dependency is introduced. The 17 legal projections change only their
Cargo-lock digest. Runtime dependency closure and rebuilt-artifact size checks passed;
their source-bound receipts are recorded below. This report makes no numerical speed, memory or artifact-size improvement claim.

### Build identity and local environment corrections

The initial release comparator reused a shared-cache binary whose compile-time
`CARGO_MANIFEST_DIR` resolved an earlier experiment clone. That pass was rejected
as current-source evidence. Rebuilding the release xtask, renderer, facade,
exporter and bindings packages produced a passing full-corpus run whose reports
resolve this working tree.

The first Web input manifests similarly listed 620 renderer files from the old
experiment clone, including the retired cutover module. Its size/smoke/browser
results are not final cleanup receipts. The canonical `wasm-size` target cache was
cleared before rebuilding current-source packages. All five resulting manifests
were checked: none contains old experiment paths or the retired cutover file.
Full now records 639 current renderer paths, including ArchitectureTextReceipt. No source or test assertion was changed to hide
this provenance failure.

The system Binaryen 133 aborted in its own constraint pass. Canonical builds use
the already-pinned Binaryen 131. Optimization flags and budget limits remain
unchanged. The native CLI recipe initially exhausted local disk space; removing
only the regenerable `target/debug/incremental` cache restored 28 GiB and the
same recipe then passed with two build jobs.

The current native `cli-release` artifact at `d5fcff484` is **53,881,152 bytes**,
SHA-256 `d1c6a0d4ea2cd18dfa18592ccf4f0269061c510f5f856ee4f7d45671d7b93e9f`.
The descriptor-owned `dist` recipe selects all declared capabilities for
`aarch64-apple-darwin`. The binary rendered all three relocated Cyberpunk scenes
to both SVG and PNG successfully. This is a current artifact/consumer receipt,
not a matched-baseline reduction or latency claim.

## Refreshed source accounting after implementation

The final-code accounting uses HEAD `d5fcff4842c9bee84a03e5b76ba31c2464065f67`
and temporary-index tree `41829ba5b9e2506c2e8a6429de604b2cfd975794`.
It includes all other tracked/untracked source and documentation changes, excludes
this execution report itself to avoid a self-referential count, and leaves the
real index unchanged. The earlier snapshot above remains dated evidence.

Relative to frozen implementation `8664ff06`, **2,580 lines are added and 18,726
are deleted across 137 paths**, a net reduction of **16,146 lines**. Four scenario
moves contribute zero lines and no binary path changes. Relative to frozen main,
the resulting PR delta is **405,021 additions and 40,413 deletions across 1,669
paths**, still including 104 binary paths. Net PR growth falls from 380,754 to
364,608 lines, or **4.24%**. The separately added execution report contributes
its own documentation lines; these are not silently included in the reduction.

At frozen documentation HEAD `a987774ff`, including this 411-line report yields **2,991
additions and 18,726 deletions across 138 paths** relative to `8664ff06`, net **15,735 fewer
lines**. The full main-to-HEAD PR is **405,432 additions and 40,413 deletions across 1,670
paths**, net **365,019 lines**. These are historical first-batch counts; the later review and
plan corrections are outside this frozen denominator and do not count as executable savings.

| Disjoint final file class | Added | Deleted | Net | Paths |
| --- | ---: | ---: | ---: | ---: |
| Rust shared source, including inline tests | 1,327 | 12,567 | -11,240 | 74 |
| Rust independent tests and benchmark | 296 | 5,939 | -5,643 | 19 |
| Non-Rust independent tests | 15 | 27 | -12 | 3 |
| Host source and tools | 8 | 28 | -20 | 8 |
| Workspace/dependency metadata | 0 | 47 | -47 | 3 |
| Generated legal metadata | 17 | 17 | 0 | 17 |
| Documentation and durable records | 881 | 101 | +780 | 8 |
| Static fixtures and provenance | 36 | 0 | +36 | 5 |
| **Total excluding this report** | **2,580** | **18,726** | **-16,146** | **137** |

The overlapping recognized-test view is **705 added, 7,202 deleted**, net **6,497
fewer Rust test lines**. Remaining shared Rust source and unrecognized test helpers
are **918 added, 11,304 deleted**, net **10,386 fewer lines**; this remains a
bounded classification, not exact production LOC. All 55 affected modules retain
the original 107-module ownership index. Final facade assembly is 19 additions,
32 deletions; retaining different admission behavior costs no new public API.

### Current Web artifact budget receipt

At source `d5fcff484`, all five canonical packages were rebuilt after clearing the
stale target cache and verified through their compiler-input manifests. Binaryen
131 and the unchanged canonical recipe produced the following bytes. The official
Web owner passed all **20 checks** against the unchanged budget file.

| Package | Raw | Stripped | Gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| analysis | 3,822,586 | 3,822,321 | 1,509,421 | 1,156,942 |
| ascii | 5,355,537 | 5,355,272 | 2,015,076 | 1,529,934 |
| editor | 3,935,383 | 3,935,118 | 1,554,794 | 1,187,169 |
| full | 16,345,242 | 16,344,977 | 6,115,763 | 4,519,607 |
| render | 14,441,460 | 14,441,195 | 5,461,453 | 4,043,943 |

These are current-product sizes. The prior accepted budget measurements belong to
`dd79ca584`; this cleanup did not run a matched A/A and baseline/candidate size
experiment and does not turn their difference into a causal theme-size claim.
No latency or allocation improvement is asserted.

The installed darwin-arm64 Node N-API package passed actual module and fresh-process
smoke: 30 SVG renders, 44 support queries, three complete scene recipes and three
fresh processes, with raw/resource/authoring rejection cases. Python and Apple
host consumers have their own receipts; this Node result does not attest them.

### Current Typst and Python receipts

The canonical Typst publish artifact passed the unchanged four-metric budget:
**19,629,838 raw**, **11,913,875 stripped**, **4,590,975 gzip**, and **3,408,538
Brotli bytes**. Its manifest contains no old experiment or retired source paths.
Typst 0.15.0 successfully compiled 23 package fixtures and rejected all 9 expected
compile failures. Its wasmi host also passed support=22, materialization=2 and
error=3 shared vectors. These receipts attest this local publish package, not
other Typst runtime versions or remote operating systems. The rebuilt Apple
XCFramework then passed its macOS Swift smoke: 10 preset exports per consumer,
4 Cyberpunk families, 10 invalid recipes, 3 fresh-process imports, 10 Class
cases, 2 strict Clear rejections, 6 missing-family renders and 6 unsupported
font-resource rejections.

The freshly built arm64 macOS Python UniFFI wheel was installed into a new venv.
Its real theme-authoring smoke passed shared vectors, three diagram families,
isolation, rule overrides, cold starts, three direct preset imports, three
complete migrated Cyberpunk scenes, five recipe rejections, 22 support vectors
and three budgeted operations per consumer. The wheel recipe and generated legal
report checks remained unchanged.
