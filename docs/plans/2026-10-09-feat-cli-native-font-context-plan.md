---
title: Shared Native Font Measurement Across Diagram Families
type: feat
status: revised
date: 2026-10-09
---

# Shared Native Font Measurement Across Diagram Families

## Current CLI Policy (2026-10-12)

Ordinary CLI Mermaid SVG, PNG, JPEG, and PDF output now defaults to deterministic
text measurement without a native font context or system discovery for layout.
PNG/JPEG/PDF painting can still load system fonts through the existing codec path;
this policy does not claim all binary exports avoid discovery. Default builds and
the official release profile still compile `native-fonts` and `system-fonts`; their
availability does not activate font acquisition for every invocation.

- `--font-file` or `--font-dir` automatically enables native measurement with a
  closed font set, even with `--font-measurement native`. Fixed font contents and
  configuration avoid catalogue discovery for faster, reproducible native output.
- `--font-measurement native` without explicit sources explicitly permits system
  discovery when `system-fonts` is compiled; `--font-system on` explicitly selects
  system discovery and native measurement, including fallback for explicit sources.
- `fontFamily` and `--runtime native` alone do not enable native-context discovery.
  Rust library, FFI, ASCII, and committed Rustdoc generation defaults remain unchanged.
- CLI contract 6 describes compiled font support separately from runtime defaults
  and explicit selection. See the [current CLI font guide](../../crates/merman-cli/README.md#explicit-fonts).

The earlier plan, decisions, and delivery/validation receipts below are retained
as historical records. This section supersedes their native-by-default CLI
statements without changing their measured numbers or verification results. No
new performance measurements are claimed for the revised default policy.

## Default Quality Revision (2026-10-10)

The maintainer selected actual-font measurement as the default CLI graphical
policy, including SVG and Mermaid-to-PNG/JPEG/PDF, because delivery quality
outweighs the measured explicit-font cost. The explicit-font timing checkpoint
does not predict system discovery cost; measure that path independently.

- Default/release CLI includes `native-fonts` and `system-fonts`. Without explicit
  sources, resolve a bounded system font set before layout. An explicit
  `--font-measurement deterministic` retains the compatibility route.
- Explicit sources remain closed by default; `--font-system on` opts into system
  fallback. Minimal builds without the CLI `system-fonts` capability retain
  deterministic defaults and must not inherit permission through feature union.
- System discovery bounds metadata/traversal work and snapshots selected font
  bytes on demand; do not retain every installed font or reuse operation fatal
  state/counters as a global font cache.
- Typed facade binary requests bind one measurement/painting context before
  typography admission. Conflicting contexts and a font context that would
  overwrite custom host measurement fail clearly. Raw-SVG exports remain
  painting-only, and core parser/model code does not acquire filesystem services.
- FFI/UniFFI/WASM defaults remain host-independent. Existing callback precedence
  is preserved. A future native-font binding service belongs in binding-core,
  not independently in each transport; no ABI expansion is needed for these CLI
  and facade changes.
- Default native unsupported typography fails with an explicit compatibility
  escape; do not claim true metrics after silently bypassing the policy.
- Before release, package same-font visual comparisons as durable repository
  assets and include separate compiled/active/system-discovery timing evidence
  in the changelog. Never link ignored `target/` artifacts from release notes.
  Include the maintainer's flowchart, ER, and sequence examples, with identical
  font files and inputs on both sides. Show overflow/padding and layout changes,
  not merely different font appearances. Record the OS, build profile, feature
  sets, font identity, workload, sample count, median and tail latency, and
  absolute/relative deltas. Distinguish fresh-process startup from reused-context
  measurements; do not describe a warm filesystem run as cold filesystem I/O.
  Preserve benchmark commands and raw samples in versioned evidence; link a
  concise visual/performance summary from the changelog.

This revision supersedes earlier unchanged-CLI-default and unavailable-system
discovery statements in historical verification checkpoints.

### Delivery Checkpoint (2026-10-10)

- Versioned [same-font visual evidence](../assets/native-font-measurement/README.md)
  includes the maintainer's Flowchart, ER and Sequence examples. The
  [system-discovery checkpoint](../performance/native_system_discovery_2026-10-10.md)
  contains the executable identities, reproduction script and 180 raw process
  observations. Median time is 2091.91 -> 1468.03 ms on the recorded Windows host;
  acquisition still has a material full-catalogue cost.
- Canonical CLI asset generation passed reproducible snapshots and source-package
  checks; distribution assets passed 5 tests. Independent native-only CLI, system
  CLI and native+PNG feature-union contracts passed 7, 8 and 8 tests respectively.
  Native-only export passed 27 tests; native+system export passed 37 tests.
  Typed PNG/JPEG/PDF context normalization passed 14 tests including reversed
  builder-order explicit-policy protection.
- Binding-core host-policy regression passed 3 selected tests and UniFFI passed
  4; browser text-measurement session tests passed 6. Release recipe/archive
  Python contracts passed 92 tests with 4 platform-conditional skips.
- Formatting, default CLI all-target Clippy and native-only export Clippy passed.
  The shared feature-matrix verifier required updates to the system capability
  allowlist and CLI recipe test constants; isolated consumers and all 42
  non-strict host build combinations passed. Cross-platform CI now includes separate system-font
  and codec feature-union lanes; their remote results are not claimed here.
- Umbrella generated verification initially found local
  `repo-ref/dompurify` 3.4.12 instead of the locked 3.4.16. Generating from the
  locked reference CLI's installed 3.4.16 source matches the committed table
  exactly. After verifying a clean reference checkout, fast-forward ancestry and
  zero local-only commits, the reference was advanced with `merge --ff-only` to
  the locked 3.4.16 commit; umbrella `verify-generated` then passed. Sanitizer
  defaults are unchanged. Binding contract projections have been regenerated.

This is a progress receipt, not the definition-of-done verdict. Remaining
publication gates must be checked against the final worktree before completion.

### Expanded Local Validation (2026-10-10)

- All-compiled-family native facade, Flowchart typography, SVG/HTML wrapping
  and typed codec context tests passed 28/28. Shared renderer text and HTML
  fallback tests passed 183/183. Release CLI native-font, command, mmdc,
  distribution and callable-profile contracts passed 69/69.
- Text-measurement protocol and binding-contract projection verifiers passed.
  Feature-matrix unit tests passed 19/19, including the explicit requirement
  that CLI defaults and cargo-dist cannot jointly omit native/system fonts.
- All 13 generated Rust license source reports passed; third-party source
  licensing passed. Four stale legal projections were synchronized with the
  existing generator; all 382 projections and 18 legal-generator tests passed.
- Full SVG DOM structure, parity and parity-root comparisons passed with only
  previously registered exact browser-text-layout/parser-diagnostic residuals;
  no comparator change or new waiver. Ten root-contract tests passed. No
  cross-platform or umbrella strict-release pass is inferred from these results.
- Native-font plus PNG/JPEG/PDF export tests passed 97/97. Export and default
  CLI all-target Clippy passed after moving system-font helpers before the test
  module; this is a source-order-only lint repair, not another performance change.
- Runtime dependency closures passed for all 36 artifact profiles using one
  descriptor-selected representative target per profile. This is dependency
  resolution evidence, not cross-platform compilation or execution evidence.
  Legal-material package listings passed for all 23 governed Cargo packages.
  Native-font plus PNG/JPEG/PDF facade documentation tests passed 1/1.
- The no-default native-only Flowchart typography and wrapping suite passed
  5/5. The no-default PNG/JPEG/PDF facade passed `cargo check --all-targets`
  without `native-fonts`, preserving independent codec compilation. Repository
  CI selection, release-workflow security, Android emulator and fuzz lifecycle
  contract suites passed 89/89; these do not substitute for remote CI execution.
- The real process matrix passed all 24 exact CLI feature profiles, including
  default and explicit release recipes. The two full debug-profile cases took
  approximately 163 seconds each; this is validation cost, not release timing.
  An explicit no-default facade lane with `diagram-flowchart,native-fonts,png,jpeg,pdf`
  now runs on every CI host, and its 14 context contracts passed locally. Remote
  results remain outstanding. Umbrella generated verification also passed after
  the approved reference checkout switch.
- Historical compiled/inactive/explicit-font raw samples and the original
  experiment ledger are now versioned under
  `docs/performance/evidence/cli-native-default-2026-10-10/`, with recomputed
  nearest-rank p95 values. The old script, exact input bytes and uncommitted
  source snapshots were not retained; archival does not establish complete
  reproduction. The system-discovery optimization has its separate versioned
  timing harness and inputs. Native ABI projection verification passed.
- Platform binding verification passed the exact Android JNI and Flutter
  Android FFI `aarch64-linux-android` native-distribution Clippy recipes with
  warnings denied. It then stopped at Flutter `pub get`: the installed Dart
  3.9.2 is below the package's `>=3.10.0 <4.0.0` requirement. Flutter package,
  generated-ABI and native-assets smoke checks were therefore not executed by
  this invocation. The pinned Flutter 3.44.9 CI lane owns those checks; no SDK
  constraint was weakened and no global SDK upgrade was performed.
- Actual CLI inspection found the native capability description still advertised
  title-only measurement. The source description and generator-owned projections
  now describe shared text/wrapping/bounds/advance support; actual release CLI
  output, capability verification and umbrella generated verification passed.
  A new same-binary explicit-font checkpoint retains the full harness, config,
  inputs and 540 raw observations for Flowchart/ER/Sequence. See
  `docs/performance/cli_explicit_font_current_2026-10-10.md`; the measured median
  whole-process increments are 6.01/3.15/2.80 ms, separate from system discovery.
- Materialized reference verification initially failed because the clean Mermaid
  checkout was three commits ahead of selected
  `21f72f07ea22c0af48a3149c550654e80d8e40cb`. After explicit maintainer approval,
  `git switch --detach` selected that commit; the existing `develop` branch
  remains at `97b345154f2cd71f23a2aadb14af6dad46f63173`. Materialized verification
  then passed. The missing ZenUML source was materialized at selected
  `38404ccc14243ed54ab45b804b2eb6f2ca73af36`; Playground installed packages match
  the lock/reference identity. No reset, restore or stash was used.

## Completion Audit (2026-10-10)

The latest local evidence is not a release-complete verdict. The original
cross-platform controlled-font requirement remains in force.

| Requirement | Current evidence | Remaining gate |
| --- | --- | --- |
| R1, R2, R4: additive native/system features, explicit closed sources, host-independent library/FFI defaults | Native-only and system suites; codec feature-union tests; 24 real CLI profiles; all descriptor target dependency closures passed | Actual controlled-font execution on Linux/macOS remains unverified |
| R3, R6: matching selected bytes, shared control, bounded acquisition/residency/probes, terminal failures | Export 97-test suite; facade context 14-test recipe; CLI batch publication/cancellation contracts; system snapshot SHA and active-control tests | Remote execution of the same controlled contracts |
| R5, R9: shared 22-operation backend, native wrapping and explicit residuals | Operation matrix, primitive/advance/whitespace tests, shared text/fallback 183-test suite | Arbitrary HTML/CSS and complete-strict measurement are excluded, not pending implementation |
| R7: truthful capabilities and selected faces | Corrected generator source, actual CLI output, capability and umbrella generated verifiers; release CLI contracts 37/37 after description change | No remaining local description mismatch found |
| R8, R10, R11: controlled selection and all-family routing | All-family native suites, Flowchart/ER/Sequence geometry and same-font visual evidence | Three-platform controlled-font CI results |
| Performance and changelog | Versioned system A/B 180 samples; current explicit-font 540 samples with full harness/config/inputs; same-font PNG/SVG assets linked from changelog | Host-dependent timing is not a cross-host guarantee; historical compiled-cost source reconstruction is unavailable and explicitly bounded |
| Generated, package, legal and dependency policy | Materialized reference and generated verifiers; 23 source-package listings; 13 legal reports and 382 projections; all-target closure verifier; `cargo deny check advisories bans licenses sources` passed | Cargo-deny duplicate-version warnings remain nonblocking under existing policy |
| Platform binding package checks | Android JNI and Android FFI native-distribution Clippy passed | Flutter package/ABI/native-assets smoke stopped at Dart 3.9.2 versus required >=3.10; pinned Flutter 3.44.9 CI owns completion |

A read-only `gh run list --branch feat/cli-native-font-context --limit 5`
query returned no workflow runs. No specific remote job is currently being
waited on. Publishing this work to trigger CI requires maintainer approval for
the scoped commits and push; no commit, push or release action has been performed.
Do not mark the goal complete before the missing execution evidence is obtained.

## Scope Correction

The maintainer clarified that the deliverable is shared actual-font measurement
for diagram text, including node labels, not a title-specific improvement.
This revision supersedes the previous title-only first-release gate. Preserve
the useful acquisition/export work already present in the worktree, but do not
describe the existing title-only adapter as completion of this plan.

## Product Contract

Add an opt-in general native font measurement backend to Merman's existing
text-measurement infrastructure, exposed through the CLI and Rust facade.
Every compiled graphical family must use it through shared routing for the
measurement operations its text layout requires. Preserve
the host-independent library measurement and existing SVG pipeline defaults.
CLI graphical defaults follow the quality policy above rather than the former
font-agnostic default.

| ID | Requirement |
| --- | --- |
| R1 | Gate the new context behind an additive `native-fonts` Cargo feature. |
| R2 | Load explicit font files/directories without requiring system discovery or binary output features. |
| R3 | Share actual font data and resolver policy across layout, label conversion, and native painting. |
| R4 | Keep system discovery a separate compile capability and independently controlled runtime policy: automatic for the full CLI, explicit opt-in for library contexts. |
| R5 | Preserve unsupported-operation fallback provenance; never substitute bbox for every advance operation. |
| R6 | Bound input font bytes, discovery, resident data, measurement work, and cache lifetime; preserve cancellation. |
| R7 | Report effective compiled capabilities and actual selected fonts truthfully. |
| R8 | Verify reproducible explicit-font selection separately from host-dependent system-font behavior. |
| R9 | Provide shared native text width/height, advance, bounds, and wrapping support through existing measurement contracts, not diagram-specific font algorithms. |
| R10 | Audit all compiled graphical families for session routing, effective style, and emitted line consistency; fix bypasses at their integration boundary. |
| R11 | Admit the capability through an operation coverage matrix and a cross-family test matrix; title-only success is insufficient. |

Non-goals: per-platform measurement engines, new provider registries, new text
layout libraries, generated browser tables, bundled production fonts, network
font acquisition, arbitrary HTML/CSS support, and Chromium pixel parity.
Complete strict measurement and text-to-path SVG are not first-release promises.

## Evidence

- `docs/research/2026-10-09-native-font-measurement-feasibility.md` records the
  public host integration experiment, title containment improvement, cache
  evidence, protocol gaps, and untested cases.
- `docs/research/2026-10-09-cross-platform-font-strategy.md` records the shared
  portable stack and explicit-font reproducibility rationale.
- `docs/research/2026-10-09-native-font-implementation-verification.md` records
  the existing uncommitted acquisition/export and title-only implementation.
  Its successful tests do not establish general label measurement coverage.
- `crates/merman-render/src/text/measure.rs` already defines shared width,
  height, advance, bounds, and wrapped measurement contracts.
- `crates/merman-render/src/environment.rs` owns policy, four phase routes,
  session provenance, and controlled measurement. `family.rs` dispatches
  layout through the session's measurer across graphical families.
- `crates/merman-render/src/text/wrap.rs` and related text helpers already
  delegate width decisions to an active measurer. Reuse these semantics.
- The current worktree separates usvg system discovery forwarding and shares
  raster/PDF resolver policy. Legacy codecs retain discovery capability;
  Cargo feature unification cannot negate it.

## Feature Contract

| Crate | Feature | Required forwarding / behavior |
| --- | --- | --- |
| `merman-export` | `native-fonts` | Optional usvg text, renderer host types, existing resource/error support, and font identity hashing; no resvg, krilla, or image encoder requirement. |
| `merman-export` | `system-fonts` | Forward usvg system discovery. Legacy PNG/JPEG/PDF continue selecting this capability to preserve existing behavior. |
| `merman` | `native-fonts` | Enable `svg`, export dependency, and `merman-export/native-fonts`; no optional output format. |
| `merman` | `system-fonts` | Add native context plus export discovery forwarding; document feature unification with legacy outputs. |
| `merman-cli` | `native-fonts` | Enable SVG, facade forwarding, and native-font CLI controls; combined with default/release `system-fonts` for quality-first graphical measurement. |
| `merman-cli` | `system-fonts` | Add native context and facade discovery forwarding; included in default/release CLI builds, with explicit sources closed unless system fallback is requested. |

Keep low-level defaults empty. Move usvg's unconditional discovery request to
feature forwarding while preserving legacy output closure. Audit every direct
dependency and cfg site before claiming an explicit-only artifact excludes
system discovery. Runtime `system=off` does not prove compile-time absence.
The facade export module must be accessible with `native-fonts` even when all
binary output features are absent. Do not enable encoding dependencies to make
that module compile.

## Architecture

Keep font ownership in a focused module inside `merman-export`. Its expanded
contract includes reusable native font services plus validated SVG encoding;
encoders must still accept only `ResvgCompatibleSvg`, never Mermaid source.
No dependency from export back to the facade is permitted.

Build one operation-owned font context before layout. It supplies a
`HostTextMeasurer` and the same immutable font/resolver configuration to the
native encoder. Install its measurement policy into the existing environment,
including Wrap, so foreignObject conversion does not create a second measurer.
Share the finalized context safely within one batch invocation, with bounded
caches and no process-global accumulation of user font data.

Implement one portable backend by measurement semantics, never by diagram ID or
label role. Map the existing operation vocabulary onto a small internal set of
font primitives: shaped-run advance, font/line metrics, ink bounds, and anchor
offsets. Keep those quantities distinct. Use usvg public layout for compatible
bounds/selection; where advance is not exposed, investigate the existing
rustybuzz dependency with the same selected faces, shaping parameters, and
fallback runs. Do not pretend an ink bbox is advance. Selection and shaping
agreement with export is an admission condition, not an assumption.

Reuse Merman's wrapping, whitespace normalization, Markdown/HTML run extraction,
and effective style handling, replacing their measurement dependency rather
than introducing a second generic line breaker. Verify emitted lines use the
same decisions as layout. The native backend must not call deterministic
measurement internally and conceal that under native provenance.

Enable native routing across Layout, Wrap, SvgBBox, and ComputedLength as their
operations are admitted. Audit session injection through `LayoutExecution`,
family preparation, SVG emission, and foreignObject conversion. Family-local
changes may repair bypassed routing or incomplete style propagation; they must
not introduce local font engines or fixture-specific widths. Static string
normalization helpers and test-only deterministic measurers are not routing
bypasses merely because their type name includes DeterministicTextMeasurer.

Fixed vertical helpers and approximate HTML metrics need an explicit inventory:
replace them through shared existing operations where possible; otherwise
identify a protocol gap, review version/binding compatibility before extending
it, and document the exact residual. Do not fix geometry by arbitrary margins.
The font context owns resources; the active render session owns operation
control. Ensure terminal host failures reach the session's fallible boundaries
without relying only on a separate acquisition control or a CLI-local check.

Retain selected font bytes safely in memory and cache compact results rather
than complete parsed SVG trees. Keys include effective style, text, operation,
wrap semantics, width, and font-context identity. Do not scale normalized
results across sizes without accounting for optical-size axes.

## CLI And Source Policy

Proposed controls: `--font-file PATH`, `--font-dir PATH` (repeatable),
`--font-system off|on`, and `--font-measurement deterministic|native`.
These controls already exist in the uncommitted implementation; revise their
help and capability coverage when the general backend is admitted.
Explicit sources opt into native measurement. The
`--font-measurement deterministic` combination with explicit sources is rejected
for Mermaid input rather than becoming an undocumented painting-only mode.
Explicit sources default to system discovery off. Default/release CLI native
measurement without explicit sources uses bounded system discovery automatically.
Missing requested capability or an empty effective font set is an error. Minimal
builds without `system-fonts` retain compatibility defaults. Library feature
compilation alone never authorizes discovery.

Acquire explicit sources in argument order; sort directory entries by a defined
stable relative-path order. Earlier explicit source wins equal face descriptors;
explicit sources outrank discovered system faces. Record file-byte hashes and
collection indices; fontFamily selects among loaded faces, not files. Define
generic-family mapping and fallback order without arbitrary first-face behavior
in explicit-only contexts. Empty/unusable explicit sets are acquisition errors.

Directory acquisition must bound traversal, files, bytes, and face counts, define
symlink handling, and avoid following cycles. Errors, cancellation, and resource
limits must abort publication instead of becoming measurement fallbacks.
Native best-effort mode records unsupported probes and missing glyphs; it does
not advertise complete strict coverage. No ambient discovery when off.

New system discovery has a separate admission gate: fontdb's bulk
`load_system_fonts()` and `load_fonts_dir()` do not expose per-file budget or
cancellation hooks. Do not call them and claim discovery is bounded by checks
performed after loading. Start with explicit files and controlled directory
walking. Admit new system scanning only after a small discovery adapter can
enumerate documented platform font roots and reuse that bounded walker, or an
upstream enumeration hook supplies equivalent control. Do not build a fontconfig
parser or an OS font engine for this. If admission fails, defer the new
`system-fonts` runtime mode while preserving legacy no-context export discovery.

For existing SVG input, supplied fonts apply to conversion/painting only; do not
claim the CLI can relayout node geometry from a finished SVG. Plain SVG remains
dependent on fonts installed at its eventual consumer.
Reject `--font-measurement` on existing SVG input. Allow explicit fonts for its
PNG/JPEG/PDF conversion, but reject them for SVG-to-SVG output in the first
release, where no font embedding or relayout is promised. For Mermaid-to-SVG,
explicit native fonts affect measured geometry but are not embedded; document
that its consumer must supply compatible fonts.

## Shared Measurement Admission

Inventory the original 19 operations in the generated text-measurement ABI before
implementation. The coverage table must record native interpretation, result
kind, required primitives, calling families/helpers, fallback conditions, and
tests. Keep existing external operation names/codes stable; update their source
and generators only if a genuine protocol gap requires a reviewed extension.

Implementation identified missing direct-text baseline facts. Append signed-length
operations `RawBBoxYOffset` (19), `RawMiddleBBoxYOffset` (20), and
`RawHangingBBoxYOffset` (21), preserving existing codes. Their optional Rust
profile methods return `None` when unsupported, so deterministic compatibility
bounds stay intact. Native and browser hosts measure the corresponding direct
text DOM; formatted-text offsets must not substitute for these operations.

| Operation group | Required contract |
| --- | --- |
| `Measure`, `MermaidCalculateTextDimensions` | Actual-font text width and height for node/label sizing, respecting existing normalization and effective style. |
| `ComputedLength`, `CanvasMeasureTextWidth` | Native shaped advance using shared face/fallback selection; no bbox substitution or browser-pixel equivalence claim. |
| Horizontal bbox operations, including `TitleBBoxX` | Derive bounds from shared font primitives and the operation's anchor/run semantics; preserve requested result kinds. |
| Raw/simple/tspan height and create-text offsets | Shared vertical metrics and baseline/anchor semantics; list remaining protocol gaps instead of hiding fixed estimates. |
| `Wrapped`, `WrappedWithRawWidth`, wrap-probe width | Reuse existing wrap rules with native widths; layout size, raw width, line count, and emitted lines must agree. |
| `BoundingClientRectWidth`, mixed HTML/CSS probes | Explicit native label-box interpretation for the supported subset; unsupported browser-only semantics must report fallback, not fabricated exactness. |

Implement incrementally by operation groups, but general single-line node text,
native-width wrapping, and multi-family routing are release requirements, not
optional follow-ups. A single title or diagram succeeding cannot admit the
backend. All compiled graphical families must be audited and receive shared
capability; fixed-layout families need not resize fixed geometry, but their
text probes must not silently bypass the selected backend.

Retain the title-overflow regression as one test, not the product boundary.
Publish a truthful coverage/residual matrix; arbitrary browser HTML/CSS,
complete-strict measurement, and font embedding remain outside scope.

## Implementation Sequence

| Step | Ownership | Work and acceptance |
| --- | --- | --- |
| 1 | shared text contracts, generated ABI, family dispatch and SVG helpers | Produce operation and family-routing matrices. Trace effective styles, wraps, fixed metrics, and session control; distinguish actual bypasses from test/normalization helpers. |
| 2 | export fonts module and existing feature/context integration | Retain and verify acquisition, residency, priority, identity, budgets, and shared export selection. Correct title-only naming and ownership assumptions; defer system discovery behind its separate gate. |
| 3 | shared native font primitives and host operation adapter | Implement single-line width/height, shaped advance, bounds and offsets through shared primitives. Verify selection/shaping matches painting before widening coverage. |
| 4 | shared text/wrap helpers and four-phase session routing | Reuse wrap and run-processing infrastructure with native widths; unify measured/emitted lines. Propagate fatal limits/cancellation through canonical session boundaries. |
| 5 | all graphical family integration boundaries | Repair audited bypasses/style gaps. Verify every compiled family uses shared routing where it measures text; no per-family font algorithms or forced pixel tuning. |
| 6 | CLI, capabilities, fixtures, profiles and docs | Replace title-only coverage reporting, exercise cross-family/resource/performance admission, document residuals and reproducibility. Default activation remains a separate decision. |

Dependencies: inventory precedes semantic implementation; primitives precede
wrap integration; family admission follows both. Existing uncommitted code is
input to inspect and adapt, not a set of completed units. Font-independent
resource/loading/export work should be retained rather than discarded.
No complete-strict CLI mode until all required operations and styles have
truthful handling; explicit-only sources alone do not establish it.

## Verification

Use focused Rust unit/integration tests and nextest, not source-parsing scripts.
New licensed fixture fonts must have committed license/provenance and cover
regular, bold, italic, collection indices, and a controlled fallback pair.

| Build / workflow | Required evidence |
| --- | --- |
| Library default / native feature off | Host-independent measurement contracts unchanged; legacy codec discovery remains painting-only. |
| SVG + native-fonts, no output codecs | Explicit files load; usvg text available; encoders absent. |
| SVG + native-fonts, system capability absent | Source-off works; source-on errors; exact profile proves absence. |
| Native + system-fonts | Bounded discovery, snapshot consistency, cancellation and limits pass; default CLI measurement is native, deterministic is an explicit opt-out. |
| Native + PNG, JPEG, PDF separately | Same selected faces/resolvers used for measurement and painting; formats validated separately. |
| Downstream feature union | Capability reports describe actual compiled union, not local manifest intent. |

Test title overflow, near-wrap thresholds, CJK/Latin fallback, combining marks,
RTL, empty/all-space text, missing glyphs, duplicate families, invalid fonts,
font-size/weight/style changes, and explicit CSS before fallback conversion.
Existing line-height and mixed HTML-run limitations remain separately classified.
The current implementation rejects custom CSS with native Mermaid measurement.
Preserve that explicit limitation until effective typography can be carried
through the shared contract; do not remove it just to widen the feature claim.

Add backend contract tests for each admitted operation, with independent
advance/metric/ink expectations. Add all-compiled-family routing tests using a
recording measurer: document families with no probes or fixed geometry, and
inspect meaningful labels rather than merely counting title callbacks.
Instrumentation with a deterministic backend alone is not native coverage.

Controlled-font integration must include Flowchart nodes/edges, Class members,
ER entities/attributes, Sequence participants/messages/notes, and at least one
computed-length/canvas consumer identified by the inventory. Other graphical
families require minimal label fixtures and an audited operation route or an
explicit no-measurement case. Changing fonts must change measured geometry
and, where text-driven, node dimensions and wrapping. Assert face agreement
and painted containment; keep intentionally fixed geometry fixed. Test both
default HTML labels and SVG text, plus resvg-safe conversion. Layout and
emission must agree on line breaks at threshold widths.
Verify bounded allocation, long unique-label workloads, canceled discovery and
measurement, and failure without partial output. Raw SVG font options cannot
change historical layout; tests must distinguish conversion from relayout.

Run the controlled explicit-font subset on Windows, Linux, and macOS. Assert
selected-face identities and bounded geometry differences, not unconditional
byte/pixel identity. Use system-font tests only for discovery invariants.
Measure cold discovery, resident bytes, warm unique/repeated probes, long wrap
inputs, whole-diagram latency, and peak memory. Research microbenchmarks are
not release thresholds; record comparable baseline/variant evidence first.

Update `capabilities/artifact-profiles-v2.json`, capability source/generation,
feature documentation and package recipes through existing tooling. Run scoped
nextest, `cargo fmt --check`, and applicable artifact-profile verification during
implementation; no tests are represented as run by this plan.

## Risks And Rollback

- Closed-font mode must not promise accuracy for unsupported protocol operations.
- Runtime CSS after layout cannot automatically repair already-sized nodes.
- System font discovery and font files are untrusted inputs with bounded budgets.
- Sharing a database without sharing resolver behavior is insufficient.
- Keep library defaults host-independent. CLI users can select
  `--font-measurement deterministic` for compatibility; slim builds can omit
  `system-fonts`. Neither path changes user source or fixture normalization.

## Definition Of Done

- Native support is organized by shared operations, with no title/family-only
  gate or diagram-ID condition inside the font backend.
- General text width/height, advance, bounds, and native-width wrapping meet
  the admission matrix; unsupported semantics have explicit residuals.
- Every compiled graphical family has a reviewed routing/typography result,
  and core cross-family fixtures prove meaningful text-driven geometry changes.
- Measurement and painting agree on faces, styles, fallback runs, and emitted
  lines; terminal resource/cancellation failures cannot publish fallback output.
- Feature/default/profile contracts, cross-platform controlled-font checks,
  resource bounds, and representative performance evidence are verified.
- Help, capability reports, and documentation describe actual operation coverage
  rather than inferring it from a title test or a font-loading flag.

## Planning Status

Revised on 2026-10-09 following the maintainer's shared-infrastructure scope
clarification. Earlier review and title-only validation do not count as review
or verification of the revised general backend. Existing source changes remain
uncommitted; this revision changes the plan only.

Confidence: high in the existing session/measurement extension point and font
acquisition/export integration; medium in advance/shaping agreement,
wrap/emission consistency, and vertical/HTML mapping. Step 1 resolves those
concrete questions. The release gate cannot be reduced to a title-only result.
Formal document review of this revised scope has not run; the current host
provides no direct skill-invocation interface for ce-doc-review. This revision
does not reuse the previous scope's reviewed status. No implementation tests
were executed while revising this document.

## Historical Opt-In Implementation Follow-Up

The opt-in shared backend, native-width wrapping, all-family session routing,
bounded explicit acquisition/export context, active control propagation and
22-operation protocol extension were implemented on 2026-10-10. This is no
longer a title-only slice. Controlled output tests cover SVG and HTML-to-resvg
wrapping; native typography admission rejects styles that the shared protocol
cannot represent or that existing compatibility layout/CSS interpret differently.

See `docs/research/2026-10-09-native-font-implementation-verification.md` for the
operation/family matrix, exact local verification and performance evidence.
This records the earlier opt-in implementation, not current CLI policy. Default
CLI activation and bounded system discovery were subsequently implemented and
locally validated as recorded above. The Linux/macOS/Windows CI matrix includes
controlled-font suites, but remote cross-platform results have not run in this
local session. Arbitrary HTML/CSS and font embedding remain out of scope;
cross-platform release admission remains unverified. Changes remain uncommitted
pending maintainer approval.


## PR Integration Checkpoint — 2026-10-12

The maintainer authorized committing, integrating remote main, deep subagent review,
fixing confirmed findings and opening the PR. The implementation is committed in
`cee23328b`; remote main `54c5c6dfb` is integrated by merge `728397269`. Subsequent
review fixes stabilize fallback selection, enforce delivery contracts, preserve
binding whitespace, share fallible wrapping, align decoded HTML/Markdown admission,
and isolate per-diagram font state while protecting directory inputs. This supersedes
earlier statements
that the implementation remains uncommitted pending approval.

See [the integration review evidence](../research/2026-10-12-native-font-pr-review.md)
for source-backed corrections, local results and checks owned by remote CI.
