# Maintenance and Mermaid parity audit closure

Date: 2026-11-04. Branch: `refactor/fearless-maintenance-20261103`.

This follow-up audits the two maintenance rounds from
`0649021aa9bca54add6911b3551d4228943c0987` to
`d8b904987aad6baa95ce917653794323d338799e`, then repairs confirmed findings
and older defects in the represented ELK surface. The original review covers
1,536 paths and concludes **Ready with fixes**, with three confirmed findings;
that historical verdict is not rewritten after the repairs.

The maintainer authorizes breaking alpha changes, local commits, environment
preparation and independent Codex agents. No external delivery is performed.
The [execution plan](../plans/2026-11-03-fearless-maintenance-and-mermaid-12-3.md)
contains the reusable investigation/implementation prompt. The
[maintenance article](https://developers.openai.com/blog/maintaining-software-with-the-gpt-6-family)
informs consumer tracing, deletion evidence and independent challenges; it does
not establish that fewer lines imply correctness or a measured speedup.

## Confirmed review findings

| Finding | Repair and independent evidence |
| --- | --- |
| CI still checks out the old Mermaid source directory | Both jobs consume validated path and commit outputs from the lock. A temporary fresh checkout containing only the selected directory reproduces both producers' exact sparse checkout and seven source blobs; workflow contracts, actionlint and zizmor pass. No hosted CI run is claimed. |
| Step motion normalization loses the owning edge route | Normal edges project coordinates only after owner, highlight and motion routes agree. Rough strokes retain raw motion geometry. A deliberate joint motion/highlight displacement was accepted before the fix and is rejected afterwards; moving the actual owner together remains valid. |
| Parser differential does not observe authored Step metadata | The actual Mermaid DB observer now includes `step`; local projection excludes render-derived Step fields. Cold/reused/reset, invalid/repeated values and binary64 boundaries have fixed corpus cases. Number normalization is confined to Flowchart edge `step`, not arbitrary observations. |

The broad review uses thirteen independent lenses followed by a separate finding
validator. Focused follow-up reviewers challenge each repair and the additional
ELK source translations. Failed proposals are corrected before acceptance, rather
than converted into comparator exceptions.

## Older ELK gaps and release diff coverage

The [ELK delta record](../../crates/merman-elk-layered/MERMAID_12_3_DELTA.md)
names exact Mermaid, elkjs and Java comparison commits. The release comparison
covers P1–P5, model order, loops, importer, compound scheduling, node spacing,
component packing, providers and adapter boundaries. Unchanged Java algorithm
bodies do not certify the historical Rust port. The published `elkjs@0.12.0`
worker decides executable expectations; its build is not uniquely attributable
to the Java release tag because elkjs CI clones ELK master.

The repairs include:

- Fractional weighted flat scoring and fixed-port target resolution, preserving
  the worker's node-influence activation condition and random-attempt schedule.
  Hierarchy scoring retains caller work control. Port comparisons charge actual
  reference-layer prefixes before access, including same-layer LongEdge scans.
- Sequenced north/south self-loop inline mode and outward label grouping, with
  final label coordinates checked by edge identity.
- Binary64 component packing in Layered, Force and Mr. Tree, plus the actual
  widened-float Box default aspect ratio. Explicit values and fallback values
  remain distinct.
- Rectpacking title-grid minima only in regular packing, after its early Box
  decision. One tall-title child yields height 210; three equal children select
  Box and yield height 175.
- Independent Layered scope direction, fixed Flowchart group padding 8 and
  separately scheduled Radial-title margins consumed by parent SPOrE.
- Empty-string `LLabel` filtering, whitespace preservation and graph flags based
  on imported edge labels. Raw owner-title dimensions still determine padding
  and minimum before text filtering. A 120-wide empty title produces a 130-wide
  owner and child y=27; an absent title produces width 64 and child y=12.
- Shared default edge thickness 1 for ordinary, compound-segment and restored
  hierarchical-port helper edges. A 10-high non-inline label needs a 14-high
  dummy with the selected spacing.
- All twelve public outside-node-label positions, label grouping, direction
  restoration and margins; Fixed and inside placement retain their contracts.

The package-owned collector runs **227** inputs twice through the unmodified
public worker and once through a phase observer; all three geometry results
agree. A replay of those exact inputs through the retained 0.9.3 package finds
**180** identical geometric projections and **47** changed ones. This separates
release changes from older port gaps. Finite observations do not prove every
possible graph or every unexposed ELK algorithm equivalent.

The selected minified worker SHA-256 is
`e4f5c51c6f2fd564bb64e52e81c4a85a316ef2f24f8b90e1db5eb886c765b030`;
the observed pretty worker SHA-256 is
`f7617e622f565748e1c88a9f45d97b458bc9c3f4299c0fdc6ed778a92ef30683`.
Full raw observations are regenerated by
`crates/merman-elk-layered/tests/elkjs_0_12_0_oracle.cjs`; they remain ignored
execution artifacts rather than a second selected-reference descriptor.

## Parser and residual follow-up

Differential discovery exposed Markdown string-token and comment-removal gaps.
The lexer shares the selected Jison state transitions across node, edge and
subgraph labels; the preprocess owner matches JavaScript comment whitespace and
line anchors while retaining source maps. Subsequent independent review found
three more cases: repeated unterminated Markdown recovery scans, reclassification
of a parsed STR subgraph payload as Markdown, and payload selections pointing
at delimiter text. The lexer now caches source quote-frame boundaries per parse,
checks cancellation during long scans and frame finalization, and retains exact
borrowed payload spans. Subgraphs consume that payload and token kind directly.
Five regressions and independent static review cover all three repairs; the full
core suite passes 2,263 tests. No parser-failure timing claim is inferred from
the deterministic scan-work assertions.

The workspace run also finds older Flowchart and Swimlane native-typography
tests using raw inner backticks inside Markdown labels. The actual selected
browser parser rejects those sources. Encoding the inner backticks as `&#96;`
is accepted upstream and still emits the same inline typography span. Their
thirteen native-admission tests pass without loosening the parser or typography
guard. The stadium renderer test similarly replaces an invalid empty STR with
a valid single-space STR, which the actual browser parser reduces to empty
text. Its break-only, icon-only and empty-text height assertions still pass
in both Classic and Neo.

The complete workspace diagnostic run checks all 14,116 tests without stopping
at the first failure. Besides the two repaired source-input tests, it identifies
45 stale deterministic layout snapshots: four Class, forty Flowchart and one
Usecase. The original snapshot owner regenerates exactly those paths. All 908
changed JSON leaves are geometry numbers; label text, identities and structure
are unchanged. The complete layout-snapshot regression then passes. These are
local layout expectations, not replacement upstream SVG baselines.

An independent reviewer accepts this exact refresh after auditing every JSON
change and its source ownership: 41 fixtures follow repaired compound thickness,
three follow painted title-frame floors, and one follows raw title-constrained
container space before frame contraction. Ordinary imported edges already used
thickness 1; the repaired defaults belong to compound segments and restored
helpers. Four additional public-worker counterfactuals, each replayed once,
isolate cross-hierarchy thickness and title minima. They reproduce both old and
new leaf positions, the 7213 height change from 432.5 to 437.5, and the 080
downstream shift of 5. No golden coordinate is an input to these observations;
only source topology and measured dimensions are reused. The raw evidence
SHA-256 is `c1da5e2a003025de872c66ac8612c48b0ec95b22378b3645d324644feb3b6ef7`.

The hand-drawn short Step fixture has one new exact three-mode receipt. Fresh
Mermaid capture is byte-identical to its tracked baseline. Calling the selected
runtime's own marker-offset and rounded-route functions on each SVG's points
reconstructs every motion/highlight operand exactly; browser node measurement
and deterministic nominal node bounds differ. The receipt binds input, upstream,
modes, precision and the entire local signature. Its mutation test renders the
real local fixture, validates the production receipt, then rejects a joint
motion/highlight route change. No tolerance or broad normalization is widened.

The escaped-tags fixture's existing receipt changes only its local signature.
Correct fixed group padding changes the deterministic title floor from `L+15`
to `max(L+8,L+10)`, narrowing its frame by 5. The retained previous N-API artifact
reproduces the old receipt exactly; fresh selected Mermaid capture is identical
to the tracked upstream. Independent review verifies the complete old/new DOM
delta, geometry propagation, unchanged text and unchanged residual category.

Current docs retire duplicate benchmark commands and obsolete ELK handoff lists;
published alpha.1 schema identity remains historical. Extension tests additionally
cover stop-await reconciliation without changing its production implementation.
Issue [276](https://github.com/Latias94/merman/issues/276) retains the previous
batch-accessibility repair and exact ER consumer coverage. Its original ER input
already passed before that repair; the reporter's original root cause was not
reproduced.

## Verification and limits

Focused evidence already passes: 2,263 core tests, 583 ELK/adapter tests, twenty
Step/parser-differential xtask tests, renderer metadata/padding/direction tests,
eleven actual parser-oracle tests, 148 workflow tests, workflow analyzers and
six extension lifecycle tests. The final parser differential runs 5,000 mutations
per family plus its fixed corpus: **10,342 cases**, with 7,696 upstream acceptances,
2,646 rejections and **zero mismatches**. Both retained original red cases replay
as resolved. The report SHA-256 is
`7894c043928f24a9e0ea6fa27005bedc821b3b091fa2a6c5c31c6979e4bc0175`.
These focused results supplement the final integrated pass below.

The CI-owned painted-containment command also runs on five freshly rendered
affected Flowchart fixtures in macOS Chromium 156 / Playwright 1.64.1. It finds
zero blocking failures and uses no exact paint receipts. Step is contained;
the other four results are **upstream-inherited indeterminate**, because both
SVGs have active filters. This is not proof that those four filtered paintings
are fully contained. The complete report is
`target/fearless-audit-root-selected/report.json`, with exact artifact hashes.

The new final `cargo run --locked -p xtask -- verify --strict` exits successfully
against the settled source. It covers formatting, all-feature workspace check
and all-target Clippy, 765 feature build cases, materialized references,
generated/alignment/legal contracts, **14,116 passed Rust tests** with nine
existing skips, architecture and workspace doctests, and all 37 families in
`structure`, `parity` and `parity-root` modes. Web build, contracts, runtime
smoke and prepack checks pass; Playground unit/lint/production build passes;
the browser suites pass 227 Chromium desktop and 93 Firefox/WebKit smoke cases;
VS Code passes 318 tests. Cargo builds stay serial with two build jobs, while
nextest uses four test threads for this final run. The complete log SHA-256 is
`dd11ec887de64679bb4e535f6f676b1db5c59284f65eedf817cb1b3c96e6b6bf`.

All five Web packages and the new Typst artifact pass the original raw,
stripped, gzip and Brotli budgets without adjustment:

| Artifact | Raw bytes | Stripped bytes | gzip bytes | Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| Web analysis | 4,087,935 | 4,087,670 | 1,561,370 | 1,189,228 |
| Web ASCII | 5,762,020 | 5,761,755 | 2,125,225 | 1,604,025 |
| Web editor | 4,242,051 | 4,241,786 | 1,621,805 | 1,232,223 |
| Web full | 17,360,257 | 17,359,992 | 6,413,965 | 4,721,698 |
| Web render | 15,330,580 | 15,330,315 | 5,717,727 | 4,216,508 |
| Typst | 20,692,467 | 12,602,662 | 4,769,776 | 3,530,172 |

Typst's new input digest is
`a877954cf8733aefd0c464e740907d7ad3676db5f00110a8f939e1b271477a69`;
its stripped artifact SHA-256 is
`f6ed693ff263c895265f913adddacaea0328c0ad61ef8324fa00e65246f79008`.
The package owner passes 27 support, two materialization and three error vectors.
Typst 0.15.1 then compiles 23 fixtures and verifies nine expected failures against
that fresh package. This is a local macOS host result, not hosted Linux or Windows
qualification.

The freshly compiled native-memory probe passes real operation/zero pairs at
the 1x and 100x boundaries; the existing report-contract owner verifies that
result. Its evidence class is `protocol-smoke` and outcome is
`protocol_smoke_pass`, not candidate admission or proof of a memory/latency gain.
It reuses the contract-owned build recipe and target directory.

Fresh macOS ARM64 N-API and Node-WASM builds are assembled, packed and installed
into separate new local consumer projects. Both installed public packages pass
theme authoring, math, icon registry isolation, JSON/SVG operations, resource
limits and cross-process recipe exchange. N-API's additional installed task
smoke passes cold concurrent/repeated work, synchronous warmup, override/failure
recovery, cancellation/deadline isolation and accepted work after disposal.
These are local package installations, not registry publications.

Both Node receipts bind source digest
`sha256:505391a7ea378f28c7d2bf3e810f31320f9804a48d7dd18c68fb0eddbcd62999`.
The N-API binary SHA-256 is
`6874c72982491888b2438c42b82f319542c7981934b46510e828d296669a390c`;
the Node-WASM binary SHA-256 is
`52ffc043edfbee9b477f7ef674bdca18a7f9a371c984909972a705172a6ab7bd`.
The two backends retain distinct recipes and input digests; a shared source
digest does not assert identical binaries or platform-wide qualification.

New SCC/BFS/DFS modes, Median, top-down layout and other unexposed ELK product
options are not implicitly admitted. Optional unsupported processors return typed
errors. `elkjs@0.12.1` remains an unadmitted candidate, and the independent
Tree-sitter package keeps its own baseline. Browser/font/RoughJS residuals remain
explicit; no unexecuted Windows, Linux-hosted or mobile-device result is claimed.

## Local review units

| Commit | Scope |
| --- | --- |
| `cc1f395fc` | Selected Mermaid CI source checkout path and producer/consumer tests. |
| `4c193b3fb` | Step owner-route closure, raw rough motion and exact receipt mutation. |
| `06babfb97` | Shared Flowchart token/payload ownership, controlled recovery and real parser observation. |
| `0f0c59c00` | ELK source/runtime repairs, adapter migration, regressions and scoped delta record. |
| `9c116dd96` | Extension configuration changes while stop is suspended. |
| `2b2e3e2eb` | Source-valid native Markdown typography regression. |
| `d23b7062c` | Source-valid Swimlane Markdown and empty-text stadium inputs. |
| `bcb5c6d2f` | Independently explained deterministic layout snapshot refresh. |

The integrated verification runs against these production bytes. Later prose-only
record updates do not replace generated-artifact source checks or change the
selected reference graph.
