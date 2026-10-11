# Native Font Context Verification

Updated: 2026-10-10. Host: Windows 11, x86_64-pc-windows-msvc.
Branch: `feat/cli-native-font-context`. No commit or publication performed.

## Historical Scope

This file records the explicit-font implementation and distribution checkpoint
before automatic system discovery was admitted. Its CLI default and unavailable
system-discovery statements are historical, not current product policy.
The subsequent [default-quality plan and delivery receipt](../plans/2026-10-09-feat-cli-native-font-context-plan.md#default-quality-revision-2026-10-10)
enable actual-font measurement plus bounded system discovery in full CLI builds,
while preserving host-independent library/FFI measurement defaults. See the
[system-discovery report](../performance/native_system_discovery_2026-10-10.md)
and [same-font visual evidence](../assets/native-font-measurement/README.md).
The original results below are retained without retroactive verification claims.

## CLI Default Distribution Checkpoint (2026-10-10)

CLI source defaults, cargo-dist metadata, and the exact `cli-release` recipe now
include `native-fonts`. Library defaults remain opt-in. Explicit font sources
activate measurement; a native-mode flag without sources fails closed.
Help, five completion scripts, and three command manpages describe shared text
measurement instead of the obsolete title-only scope.

Verification: 246 default CLI unit tests and 73 selected integration tests
passed; the exact release and no-default-features SVG process contracts passed
separately. Artifact validation passed for 36 profiles. Python recipe/matrix
tests passed (37 passed, 3 skipped). All-target/all-feature CLI Clippy, formatting,
and the third-party license contract passed. PowerShell completion syntax passed;
the combined shell validator was blocked by the host Bash shim's WSL mount
failure, so other shells remain a CI check.

Two Windows `release` builds used the same source and full release feature set,
differing only in `native-fonts`: 49,321,472 bytes without it and 49,551,360 bytes
with it (+229,888 bytes, +0.466%). This is not the thin-LTO `dist` profile or a
latency measurement. Default SVG output compared equal across both executables
for Flowchart, explicit `fontFamily` without sources, Sequence, Class, State,
and Pie smoke inputs. The comparison does not establish browser pixel parity.

## Implemented Scope

- Opt-in library `native-fonts` and separate `system-fonts` forwarding. As of the
  2026-10-10 distribution update, CLI defaults and the official release recipe
  include `native-fonts`; runtime measurement still requires explicit font
  sources, and no-argument measurement behavior remains unchanged.
- Explicit TTF/OTF/TTC acquisition with stable source order, bounded traversal,
  safe in-memory bytes, cancellation, residency and measurement/cache ceilings.
- Shared usvg/fontdb resolver for all 22 shared measurement operations and
  PNG/JPEG/PDF painting. The backend has no diagram-family dispatch. It measures
  advances, metric/ink bounds, vertical offsets and native-width wrapping;
  missing-glyph requests retain explicit deterministic fallback evidence.
- Shared fallible wrapping preserves existing SVG/HTML line-breaking policies,
  grapheme boundaries, emitted row offsets and operation-specific whitespace.
- CLI source and measurement controls, selected faces and content identity,
  raw-SVG conversion-only semantics, unchanged default measurement.
- Facade environment preserves fatal checking and automatically supplies its
  context to binary encoders. Native Mermaid mode rejects custom CSS and
  parser-owned typography declarations that the measurement protocol cannot
  represent. Paint-only source declarations remain supported.
- SVG vertical extents use native raw-text baseline offsets when available and
  preserve compatibility estimates with existing deterministic measurers.
- C, Web, UniFFI, Kotlin, Swift, Dart and Python vocabulary preserves operation
  codes 0..18 and appends three signed raw baseline-offset operations at 19..21.

## Operation And Family Coverage

| Shared operation group | Native interpretation | Representative route/evidence |
| --- | --- | --- |
| Measure, Wrapped, WrappedWithRawWidth | Existing grapheme/word wrap policy with actual-font advance; SVG emitted-row bbox height or HTML 1.5em line box | Flowchart nodes/edges, Class members, ER attributes, Sequence messages/notes; threshold output tests |
| ComputedLength, CanvasMeasureTextWidth | Start/end anchor difference from the same usvg tree, preserving kerning/fallback runs | Timeline wrapping, Treemap fit, Architecture labels; independent usvg/rustybuzz probes |
| BBoxX, BBoxXWithAsciiOverhang, TitleBBoxX | Centered metric extents, with ink union where the operation requests overhang | Shared SVG bounds/title helpers; independent painted containment |
| Simple/Raw/Tspan/WrapProbe width, BoundingClientRectWidth | Operation-specific normalized/preserved text metric bbox | GitGraph preserved branch labels, shared wrap probes, Pie/Journey legends |
| Simple/Raw/Tspan height, MermaidCalculateTextDimensions | Actual text bbox; drawSimpleText/calculated dimensions flatten explicit HTML breaks | GitGraph, Ishikawa and shared node-size consumers; whitespace tests |
| CreateTextBBoxYOffset, CreateTextMiddleBBoxYOffset | Formatted-label baseline offsets | Shared SVG formatted text |
| RawBBoxYOffset, RawMiddleBBoxYOffset, RawHangingBBoxYOffset | Direct-text baseline offsets, independent of formatted-label semantics | Ishikawa, Sankey, shared title vertical bounds |

The all-family fixture enumerates Agentflow, Architecture, Block, C4, Class,
Cynefin, ER, EventModeling, Flowchart, Gantt, GitGraph, Ishikawa, Journey, Kanban,
Mindmap, Pie, Railroad, Requirement, Sankey, Sequence, State, Swimlane, Timeline,
TreeView, Treemap, Usecase, Wardley, XYChart and ZenUML. It verifies native
non-title operation provenance with no fallback for its controlled Latin labels.
Info, Packet, QuadrantChart, Radar and Venn have separate fixed-geometry checks;
they do not acquire invented body-measurement calls. Error is infrastructure.
Routing evidence alone does not imply every possible label, font or style has
painted containment: focused output and primitive tests establish that stronger
property for representative supported paths.

## Observed Verification

- Explicit-only native export: 26 nextest tests passed, including independent
  usvg/rustybuzz advance checks, pair kerning, NBSP, RTL, CJK fallback, TTC index,
  malformed collection, bounded cache, whitespace and active-control failures.
- Facade and all-family integration: 15 nextest tests passed. Flowchart, Class,
  ER and Sequence layouts change with explicit font metrics. Every compiled
  text-driven family uses native non-title probes; fixed geometry remains fixed.
- Shared text suite: 124 nextest tests passed; focused routing, baseline and
  Ishikawa/GitGraph style tests: 27 passed.
- Browser measurement adapter: TypeScript compilation and six Node tests passed,
  including exact routing and signed offsets. The full Web build is blocked by
  existing WASM/TypeScript editorSemanticTokenDescriptor/editorSemanticTokens
  export drift, before its TypeScript compilation step.
- Swift UniFFI bindings were regenerated from the native-sdk library with all
  graphical diagram features.
- CLI with explicit fonts, parallel Markdown and all three codecs: eight
  nextest tests passed. Budget failure preserves existing outputs and publishes
  no new diagram or manifest; the stable empty directory lock remains by design.
- Shared binding projection: 11 focused tests passed; UniFFI exact operation
  transport: one test passed. Protocol/capability generation verification and
  all 36 artifact profiles passed after the general-backend revision.
- Default CLI binary unit suite rerun: 246 passed, one existing skipped.
- Flowchart typography admission: two tests passed; numeric/non-px mismatches
  and root weight inconsistencies fail native admission without changing the
  existing deterministic compatibility path.
- Resvg HTML fallback: 57 tests passed, including preserved break-spaces tails,
  NBSP boundaries, internal explicit empty rows, trailing-break row alignment,
  normal/nowrap behavior and cancellation. Supported HTML conversion now reuses
  the shared break-spaces wrap policy instead of trimming its measured rows.
- Actual output wrapping: three tests passed, covering SVG, explicit HTML size
  and the effective default theme size. Native metrics, emitted rows, node
  geometry and independently shaped usvg bounds agree on these fixtures.
- Final third-party license verification passed after regenerating 13 Rust
  dependency reports for the updated lockfile. Formatting and diff checks passed.

## Performance Evidence

Follow-up optimization and cold/batch results are recorded in
`docs/performance/native_font_measurement_2026-10-10.md`. The table below is the
earlier baseline before final-row metric reuse, not the latest candidate result.

Command: `cargo bench -p merman --no-default-features --features
native-fonts,diagram-flowchart --bench native_font_measurement -- --quick`.
Release build, Windows host above, checked-in Cousine Regular. Criterion quick
sampling is exploratory, not a regression threshold. Other local workloads and
CPU frequency are not controlled. New context acquisition does not flush the OS
file cache. Allocation figures are one-operation `CountingSystemAllocator`
measurements, not process RSS or timed-sample averages.

| Workload | Criterion interval | Allocated bytes | Peak growth bytes |
| --- | --- | ---: | ---: |
| New font context | 578.30–590.44 us | 1,036,261 | 526,679 |
| Cached advance, native | 590.42–618.81 ns | 351 | 196 |
| Advance, deterministic | 1.2795–1.3044 us | 286 | 239 |
| Uncached unique advance, native | 212.06–221.51 us | 405,545 | 111,205 |
| Unique advance, deterministic | 1.0920–1.1014 us | 268 | 230 |
| Long wrap, native cached probes | 763.31–770.79 us | 456,448 | 39,265 |
| Long wrap, native no cache | 163.28–167.14 ms | 283,754,706 | 133,339 |
| Long wrap, deterministic | 185.51–186.72 us | 87,321 | 39,265 |
| Whole warm Flowchart, native | 7.1081–7.2700 ms | 6,569,313 | 1,069,725 |
| Whole warm Flowchart, deterministic | 7.3378–7.5459 ms | 6,310,565 | 1,093,615 |

The first run constructed six text probes for every native miss. Deferring
baseline-specific probes reduced the same uncached long-wrap observation from
372.94–418.84 ms / 719,101,066 allocated bytes to the figures above. Quick timing
samples are noisy; allocation traffic is the clearest evidence for this change.

The long label repeats a 49-byte phrase 64 times. Native uncached probes rebuild
independent usvg text trees: transient allocation traffic is substantial even
though live memory stays bounded. Cache capacity, label/work ceilings and active
cancellation are important operational controls. This evidence is not admission
to enable native fonts by default or a claim that uncached native measurement is
as cheap as the deterministic estimator.

Earlier codec-only checks passed independently for PNG, JPEG and PDF, and an
earlier all-codec native export suite passed 65 tests. Those checks precede the
general-backend revision; the revision's reruns are listed above. The revised
explicit-only dependency tree excludes fontdb filesystem/system discovery;
adding `system-fonts` includes its discovery features as expected.

Controlled fonts include Cousine Regular/Bold/Italic, a two-face TTC and small
Noto Latin/SC subsets under SIL OFL. Pinned source commits, licenses and subset
construction are recorded alongside the fixtures. No installed host font is
required by the controlled native-font tests.

## Not Yet Verified Or Admitted

- Linux/macOS controlled-font runs and platform filesystem invariants.
  The existing three-platform CI matrix now runs explicit-font export, facade,
  typography and CLI batch suites; remote results are not represented as run.
- A broad malformed-font corpus and representative whole-diagram latency and
  peak-memory release thresholds. Current tests cover truncated TTC structure,
  explicit resource ceilings and whole-family functional workloads.
- Runtime system discovery: explicitly unavailable in the new context. Legacy
  no-context binary export retains its historical unbounded first-use discovery.
- Arbitrary HTML/CSS typography, browser pixel identity and automatic system-font
  discovery are not admitted. Native HTML-like measurement uses the existing
  shared label policy; it is not a browser CSS engine.
- SVG fonts are not embedded. Low-level host policy/custom pipeline integrations
  remain responsible for final context checking and typography consistency.

The shared native backend and cross-family integration are implemented. This
Windows verification does not establish cross-platform release admission or
change the opt-in default.
